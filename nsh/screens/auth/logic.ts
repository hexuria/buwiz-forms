// The rules of the sign-in, setup and account-gate screens (old app/main.js auth section, the old shell's auth.js, gate.js): pure.
import { deviceCopy } from "../../ui/look";

/** The TIN typed on the sign-in screen: the admin account, a taxpayer, or why neither. */
export type SignInTarget = { error: string } | { root: string; admin: boolean };

/** Whose TIN `tin` (digits) is: the admin's root, a taxpayer's root among `roots`, or why neither (old signInTarget). */
export function signInTarget(tin: string, adminRoot: string | null, roots: readonly string[]): SignInTarget {
  if (tin.length !== 9) return { error: "Type all 9 digits of the TIN." };
  if (tin === adminRoot) return { root: tin, admin: true };
  if (!roots.includes(tin)) return { error: deviceCopy("No taxpayer has that TIN on this computer.") };
  return { root: tin, admin: false };
}

/** The line under "Sign in": what to type, for the method and the "Forgot PIN?" path. */
export function signInLine(o: { forgot: boolean; forAdmin: boolean; helpdesk: boolean }): string {
  if (o.forgot) return o.forAdmin ? "Use your recovery key, or your Mac login." : "The admin can open your account with their PIN.";
  if (o.helpdesk) return "Enter your TIN and your PIN. The help desk admin signs in with TIN 000-000-000.";
  return "Enter your TIN and your PIN.";
}

/** Why the code as typed cannot be checked yet; null when it is complete. */
export function codeLengthError(totp: boolean, code: string): string | null {
  if (code.length === (totp ? 6 : 4)) return null;
  return totp ? "Type all 6 digits of the code." : "Type all 4 digits of the PIN.";
}

/** The answer to a sign-in code that does not match. */
export function wrongCode(totp: boolean, taxpayerHasNoCode: boolean): string {
  if (taxpayerHasNoCode) return "That taxpayer has no PIN yet. Ask the admin to open it.";
  return totp ? "Wrong TIN or code. Try again." : "Wrong TIN or PIN. Try again.";
}
