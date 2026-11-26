// The pure rules of the gate and lock screens, and the test authenticator against a published RFC 6238 vector.
import { expect, test } from "nsh/test";
import { codeLengthError, signInLine, signInTarget, wrongCode } from "./logic";
import { generatedSecret, totpCode } from "./testTotp";
import { deviceCopy } from "../../ui/look";

test("the TIN names the admin, a taxpayer, or says why not", () => {
  expect(signInTarget("00012", "000123453", ["000123453"])).toEqual({ error: "Type all 9 digits of the TIN." });
  expect(signInTarget("000000000", "000000000", ["000123453"])).toEqual({ root: "000000000", admin: true });
  expect(signInTarget("000123453", "000000000", ["000123453"])).toEqual({ root: "000123453", admin: false });
  expect(signInTarget("000111225", "000000000", ["000123453"])).toEqual({ error: deviceCopy("No taxpayer has that TIN on this computer.") });
});

test("the sign-in line follows the path and the mode", () => {
  expect(signInLine({ forgot: false, forAdmin: false, helpdesk: false })).toBe("Enter your TIN and your PIN.");
  expect(signInLine({ forgot: false, forAdmin: false, helpdesk: true })).toBe("Enter your TIN and your PIN. The help desk admin signs in with TIN 000-000-000.");
  expect(signInLine({ forgot: true, forAdmin: true, helpdesk: true })).toBe("Use your recovery key, or your Mac login.");
  expect(signInLine({ forgot: true, forAdmin: false, helpdesk: false })).toBe("The admin can open your account with their PIN.");
});

test("codes: length first, then which wrong answer", () => {
  expect(codeLengthError(false, "12")).toBe("Type all 4 digits of the PIN.");
  expect(codeLengthError(true, "12345")).toBe("Type all 6 digits of the code.");
  expect(codeLengthError(false, "1234")).toBe(null);
  expect(wrongCode(false, false)).toBe("Wrong TIN or PIN. Try again.");
  expect(wrongCode(true, false)).toBe("Wrong TIN or code. Try again.");
  expect(wrongCode(false, true)).toBe("That taxpayer has no PIN yet. Ask the admin to open it.");
});

test("the test authenticator gives RFC 6238's SHA-1 codes", () => {
  // RFC 6238 appendix B: the ASCII secret "12345678901234567890" (base32 below) at 59 s is 94287082 (8 digits; 6 here).
  expect(totpCode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59_000)).toBe("287082");
  expect(totpCode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 1_111_111_109_000)).toBe("081804");
  expect(generatedSecret()).toMatch(/^[A-Z2-7]{32}$/);
});
