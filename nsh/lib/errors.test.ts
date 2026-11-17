import { describe, expect, test } from "nsh/test";
import { errorText } from "./errors";

describe("errorText", () => {
  test("an Error reads as its message", () => {
    expect(errorText(new Error("database is locked"))).toBe("database is locked");
  });
  test("drops the host function name that starts the message", () => {
    expect(errorText(new Error("`list_taxpayers`: database is locked"))).toBe("database is locked");
  });
  test("drops an Error: prefix from text or from a message", () => {
    expect(errorText("Error: boom")).toBe("boom");
    expect(errorText(new Error("Error: boom"))).toBe("boom");
  });
  test("a plain value is its text", () => {
    expect(errorText("plain text")).toBe("plain text");
    expect(errorText(42)).toBe("42");
  });
  test("an object with a message reads as that message", () => {
    expect(errorText({ message: "from an object" })).toBe("from an object");
  });
  test("only the first function name is dropped", () => {
    expect(errorText("`a.b`: `c.d`: text")).toBe("`c.d`: text");
  });
  test("old behaviour kept: an empty message reads as the error's name, and null reads as the word null", () => {
    expect(errorText(new Error(""))).toBe("Error");
    expect(errorText(null)).toBe("null");
    expect(errorText(undefined)).toBe("undefined");
  });
});
