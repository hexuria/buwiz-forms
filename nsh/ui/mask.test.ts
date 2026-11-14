// The masks' pure functions: TIN, money, percent, integers, the caret rule, dates.
import { expect, test } from "nsh/test";
import {
  applyMask, DATE_ERROR, digitsOnly, formatIso, formatLongDate, formatMDY, formatMoney, formatTin, formatTinDigits, intMask,
  isPlainAmount, MONEY_MASK, moneyMask, moneyPlain, parseTypedDate, PERCENT_MASK, reconcile, sameAmount, shownDate, shownMoney,
  TIN_MASK,
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

test("money: a minus is only first, and a minus that is there is always shown", () => {
  // Wave 1 expected "1,500" here: a stored or computed negative showed positive. Typing one is what `negative` governs now.
  expect(applyMask("-1500", MONEY_MASK)).toBe("-1,500");
  expect(applyMask("-250.00", MONEY_MASK)).toBe("-250.00");
  expect(applyMask("15-00", MONEY_MASK)).toBe("1,500");
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

test("TIN from root and branch: every digit given is shown, grouped 3-3-3-5", () => {
  expect(formatTin("000123453", "00000")).toBe("000-123-453-00000");
  expect(formatTin("000123453")).toBe("000-123-453");
  // Wave 1 cut these to the root ("000-123-453") and left a short root ungrouped ("1234").
  expect(formatTin("0001234530")).toBe("000-123-453-0");
  expect(formatTin("00012345300000")).toBe("000-123-453-00000");
  expect(formatTin("0001234")).toBe("000-123-4");
  expect(formatTin("1234")).toBe("123-4");
  expect(formatTin("000-123-453-0000")).toBe("000-123-453-0000");
  expect(formatTin("")).toBe("");
  expect(formatTin("00012345300000", "00001")).toBe("000-123-453-00001");
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

// ---------------------------------------------------------------- review fixes (wave 1 findings)

test("money: leaving the box rounds on the digits, half up, never through a float", () => {
  expect(formatMoney("1.255")).toBe("1.26"); // toFixed gave 1.25
  expect(formatMoney("2.675")).toBe("2.68"); // toFixed gave 2.67
  expect(formatMoney("1.005")).toBe("1.01"); // toFixed gave 1.00
  expect(formatMoney("1.004")).toBe("1.00");
  expect(formatMoney("0.995")).toBe("1.00");
  expect(formatMoney("999.995")).toBe("1,000.00");
  expect(formatMoney("1234.5678")).toBe("1,234.57");
  expect(formatMoney("-1.255")).toBe("-1.26");
  expect(formatMoney("-2.675")).toBe("-2.68");
  expect(formatMoney(".5")).toBe("0.50");
  expect(formatMoney("5.")).toBe("5.00");
  expect(formatMoney("007.5")).toBe("7.50");
});

test("money: no 'Infinity.undefined', no '-0.00', every digit of a long amount stays", () => {
  expect(formatMoney("-0")).toBe("0.00");
  expect(formatMoney("-0.00")).toBe("0.00");
  expect(formatMoney("-0.001")).toBe("0.00");
  expect(formatMoney("Infinity")).toBe("Infinity");
  expect(formatMoney("-Infinity")).toBe("-Infinity");
  expect(formatMoney("NaN")).toBe("NaN");
  expect(formatMoney("1e21")).toBe("1e21");
  expect(formatMoney("1.2.3")).toBe("1.2.3");
  expect(formatMoney("-")).toBe("-");
  expect(formatMoney(".")).toBe(".");
  expect(formatMoney("123456789012345678")).toBe("123,456,789,012,345,678.00");
  expect(formatMoney("9007199254740993.01")).toBe("9,007,199,254,740,993.01"); // past Number.MAX_SAFE_INTEGER
  expect(formatMoney("12345678901234567890.125")).toBe("12,345,678,901,234,567,890.13");
  expect(applyMask("123456789012345678.99", MONEY_MASK)).toBe("123,456,789,012,345,678.99");
  expect(formatMoney("1234.00", true)).toBe("1,234");
  expect(formatMoney("1234.", true)).toBe("1,234");
  expect(formatMoney("-0", true)).toBe("0");
  expect(formatMoney("1234.5", true)).toBe("1234.5");
});

test("money: what a box shows for a stored value (negatives shown, non-amounts verbatim)", () => {
  expect(shownMoney("-250", false)).toBe("-250.00");
  expect(shownMoney("-250", true)).toBe("-250");
  expect(shownMoney("1250000.5", true)).toBe("1,250,000.5");
  expect(shownMoney("1250000.5", false)).toBe("1,250,000.50");
  expect(shownMoney("1234.5", false, true)).toBe("1234.5");
  expect(shownMoney("-0", false)).toBe("0.00");
  expect(shownMoney("", false)).toBe("");
  // Wave 1 showed "1e21" as "121".
  expect(shownMoney("1e21", false)).toBe("1e21");
  expect(shownMoney("1e21", true)).toBe("1e21");
  expect(shownMoney("Infinity", false)).toBe("Infinity");
  expect(shownMoney("abc", true)).toBe("abc");
  expect(isPlainAmount("1,250.50")).toBe(true);
  expect(isPlainAmount("1e21")).toBe(false);
  expect(isPlainAmount("")).toBe(false);
});

test("money: two spellings of one amount are the same, exactly", () => {
  expect(sameAmount("1250.5", "1,250.50")).toBe(true);
  expect(sameAmount("0", "-0.00")).toBe(true);
  expect(sameAmount(".5", "0.50")).toBe(true);
  expect(sameAmount("", "")).toBe(true);
  expect(sameAmount("1.255", "1.26")).toBe(false);
  expect(sameAmount("12345678901234567891", "12345678901234567890")).toBe(false); // equal as floats
  expect(sameAmount("abc", "abc")).toBe(true);
  expect(sameAmount("abc", "abd")).toBe(false);
});

test("money: a typed or pasted minus is refused where negatives are not allowed, never made positive", () => {
  expect(reconcile("12", "-12", MONEY_MASK)).toEqual({ text: "12", caret: 0, rejected: true });
  expect(reconcile("12", "1-2", MONEY_MASK)).toEqual({ text: "12", caret: 1, rejected: true });
  expect(reconcile("", "-1,234.00", MONEY_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  expect(reconcile("", "₱ -1,234.00", MONEY_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  expect(reconcile("", "(1,234.00)", MONEY_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  expect(reconcile("", "\u22121,234.00", MONEY_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  // A negative that is there stays negative while its digits change.
  expect(reconcile("-250", "-2500", MONEY_MASK)).toEqual({ text: "-2,500", caret: 6 });
});

test("money: where negatives are allowed, a minus, a Unicode minus and accounting brackets all read as negative", () => {
  const neg = moneyMask({ negative: true });
  expect(reconcile("12", "-12", neg)).toEqual({ text: "-12", caret: 1 });
  expect(reconcile("", "-1,234.00", neg)).toEqual({ text: "-1,234.00", caret: 9 });
  expect(reconcile("", "(1,234.00)", neg)).toEqual({ text: "-1,234.00", caret: 9 });
  expect(reconcile("", "\u22121,234.00", neg)).toEqual({ text: "-1,234.00", caret: 9 });
  expect(reconcile("", "₱ (1,234.00)", neg)).toEqual({ text: "-1,234.00", caret: 9 });
});

test("percent and integers refuse a pasted minus too", () => {
  expect(reconcile("12", "-12", PERCENT_MASK)).toEqual({ text: "12", caret: 0, rejected: true });
  expect(reconcile("", "-5", intMask())).toEqual({ text: "", caret: 0, rejected: true });
  expect(reconcile("", "1.234.567", PERCENT_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  expect(reconcile("12", "123", PERCENT_MASK)).toEqual({ text: "123", caret: 3 });
  expect(reconcile("12", "12.5", PERCENT_MASK)).toEqual({ text: "12.5", caret: 4 });
});

test("money: separators in a paste follow one rule, an ambiguous paste is refused", () => {
  // One kind repeated groups thousands when every group after the first has three digits.
  expect(reconcile("", "1.234.567", MONEY_MASK)).toEqual({ text: "1,234,567", caret: 9 }); // was "1.234567"
  expect(reconcile("", "1,234,567", MONEY_MASK)).toEqual({ text: "1,234,567", caret: 9 });
  // Both kinds: the last one is the decimal point when it is alone of its kind, after the other kind and has 1-2 digits.
  expect(reconcile("", "1.234,56", MONEY_MASK)).toEqual({ text: "1,234.56", caret: 8 }); // was "1.23456"
  expect(reconcile("", "1,234.56", MONEY_MASK)).toEqual({ text: "1,234.56", caret: 8 });
  expect(reconcile("", "1.234.567,8", MONEY_MASK)).toEqual({ text: "1,234,567.8", caret: 11 });
  expect(reconcile("", "1,234,567.89", MONEY_MASK)).toEqual({ text: "1,234,567.89", caret: 12 });
  // Anything else with two or more separators could mean two things: refused, nothing changes.
  for (const ambiguous of ["1.2.3", "1,234.567", "1.234,567", "12,34,567", "1,2,3", "1.234.56", "1,234,56"]) {
    expect(reconcile("", ambiguous, MONEY_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  }
  // A single separator is left to the mask, as typed; typing a second point still just does nothing.
  expect(reconcile("1.5", "1.5.", MONEY_MASK)).toEqual({ text: "1.5", caret: 3 });
  expect(applyMask("1.2.3", MONEY_MASK)).toBe("1.23");
});

test("caret: Delete on a separator takes the digit after it, Backspace the digit before, given the selection", () => {
  // Without the selection the two keys leave the same texts: the guess stays Backspace.
  expect(reconcile("1,234", "1234", MONEY_MASK)).toEqual({ text: "234", caret: 0 });
  // Backspace on "1,|234".
  expect(reconcile("1,234", "1234", MONEY_MASK, undefined, { start: 2, end: 2 })).toEqual({ text: "234", caret: 0 });
  // Delete on "1|,234": was "234".
  expect(reconcile("1,234", "1234", MONEY_MASK, undefined, { start: 1, end: 1 })).toEqual({ text: "134", caret: 1 });
  // TIN: Delete on "000|-123" takes the 1, Backspace on "000-|123" the last 0.
  expect(reconcile("000-123", "000123", TIN_MASK, undefined, { start: 3, end: 3 })).toEqual({ text: "000-23", caret: 3 });
  expect(reconcile("000-123", "000123", TIN_MASK, undefined, { start: 4, end: 4 })).toEqual({ text: "001-23", caret: 2 });
  // A digit deleted either way still goes, whichever side.
  expect(reconcile("1,234", "1,24", MONEY_MASK, undefined, { start: 3, end: 3 })).toEqual({ text: "124", caret: 2 });
  expect(reconcile("1,234", "1,34", MONEY_MASK, undefined, { start: 3, end: 3 })).toEqual({ text: "134", caret: 1 });
  // A selection replaced by a typed digit, and the whole text selected.
  expect(reconcile("1,234", "9", MONEY_MASK, undefined, { start: 0, end: 5 })).toEqual({ text: "9", caret: 1 });
  expect(reconcile("1,234", "1,9", MONEY_MASK, undefined, { start: 2, end: 5 })).toEqual({ text: "19", caret: 2 });
});

test("caret: a typed digit beside an equal one lands where it was typed, given the selection", () => {
  // "000-1|23" + "2": the texts alone say the 2 went in before the 3, or after the other 2.
  expect(reconcile("000-123", "000-1223", TIN_MASK, undefined, { start: 5, end: 5 })).toEqual({ text: "000-122-3", caret: 6 });
  expect(reconcile("000-123", "000-1223", TIN_MASK, 6)).toEqual({ text: "000-122-3", caret: 6 });
  // The other reading, "000-12|3" + "2", is the guess without a hint and exact with one.
  expect(reconcile("000-123", "000-1223", TIN_MASK)).toEqual({ text: "000-122-3", caret: 7 });
  expect(reconcile("000-123", "000-1223", TIN_MASK, undefined, { start: 6, end: 6 })).toEqual({ text: "000-122-3", caret: 7 });
  // "000-0|00" + "0": the caret stays after the typed 0 (was the end of the text).
  expect(reconcile("000-000", "000-0000", TIN_MASK, undefined, { start: 5, end: 5 })).toEqual({ text: "000-000-0", caret: 6 });
  expect(reconcile("000-000", "000-0000", TIN_MASK, 6)).toEqual({ text: "000-000-0", caret: 6 });
  // "00|0-000" + "0".
  expect(reconcile("000-000", "0000-000", TIN_MASK, undefined, { start: 2, end: 2 })).toEqual({ text: "000-000-0", caret: 3 });
  // Money: "1,2|34" + "3" and "1,23|4" + "3".
  expect(reconcile("1,234", "1,2334", MONEY_MASK, undefined, { start: 3, end: 3 })).toEqual({ text: "12,334", caret: 4 });
  expect(reconcile("1,234", "1,2334", MONEY_MASK, undefined, { start: 4, end: 4 })).toEqual({ text: "12,334", caret: 5 });
});

test("caret: a selection that does not fit the two texts is ignored", () => {
  expect(reconcile("123", "1234", MONEY_MASK, undefined, { start: 1, end: 1 })).toEqual({ text: "1,234", caret: 5 });
  expect(reconcile("123", "1234", MONEY_MASK, undefined, { start: 9, end: 9 })).toEqual({ text: "1,234", caret: 5 });
  expect(reconcile("123", "1234", MONEY_MASK, undefined, { start: -1, end: 2 })).toEqual({ text: "1,234", caret: 5 });
  expect(reconcile("123", "1234", MONEY_MASK, 99)).toEqual({ text: "1,234", caret: 5 });
  expect(reconcile("1,234", "1,2345", MONEY_MASK, undefined, { start: 5, end: 5 })).toEqual({ text: "12,345", caret: 6 });
});

test("typed text is only cleaned, pasted text is judged whole", () => {
  const neg = moneyMask({ negative: true });
  // Keys that arrive together (the box has not answered the first yet): the minus goes, the digit stays.
  expect(reconcile("", "-5", MONEY_MASK, undefined, undefined, true)).toEqual({ text: "5", caret: 1 });
  expect(reconcile("12", "-12", MONEY_MASK, undefined, undefined, true)).toEqual({ text: "12", caret: 0 });
  expect(reconcile("12", "-12", neg, undefined, undefined, true)).toEqual({ text: "-12", caret: 1 });
  expect(reconcile("12", "-12", PERCENT_MASK, undefined, undefined, true)).toEqual({ text: "12", caret: 0 });
  // The same text pasted is refused whole.
  expect(reconcile("", "-5", MONEY_MASK)).toEqual({ text: "", caret: 0, rejected: true });
  // Separators typed key by key are never read as one ambiguous paste.
  expect(reconcile("12,345", "1,234,56", MONEY_MASK, undefined, undefined, true)).toEqual({ text: "123,456", caret: 7 });
  expect(reconcile("12,345", "1,234,56", MONEY_MASK)).toEqual({ text: "12,345", caret: 1, rejected: true });
});

test("money: a stored value is laid out, never rounded away; rounding is for what the filer typed", () => {
  expect(formatMoney("1.255", false, false)).toBe("1.255");
  expect(formatMoney("1234.5678", false, false)).toBe("1,234.5678");
  expect(formatMoney("1.5", false, false)).toBe("1.50");
  expect(formatMoney("-0.001", false, false)).toBe("-0.001");
  expect(shownMoney("1.255", false)).toBe("1.255");
  expect(shownMoney("1234.5678", false)).toBe("1,234.5678");
  expect(shownMoney("1234.5", false)).toBe("1,234.50");
});
