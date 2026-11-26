// The sign-in cards' layout decisions, for touch and for a pointer (signInLook.ts). The headless test API reads no sizes or
// positions, so the heights are checked here as the cards' record of the kit's, and the kit's own classes are in nsh/ui.
import { expect, test } from "nsh/test";
import { authScrollStyle, codeActionsClass, SAID_SLOT, signInHeights } from "./signInLook";

test("touch: the field, the buttons and the boxes are at least 44 points tall (the boxes 48); a pointer keeps 32", () => {
  const touch = signInHeights(true);
  expect(touch.field).toBeGreaterThanOrEqual(44);
  expect(touch.button).toBeGreaterThanOrEqual(44);
  expect(touch.box).toBeGreaterThanOrEqual(48);
  expect(signInHeights(false)).toEqual({ field: 32, button: 32, box: 32 });
});

test("touch: the two secondary actions are stacked at full width, 12 points apart; with a pointer a centred row", () => {
  expect(codeActionsClass(true)).toBe("w-full gap-3");
  expect(codeActionsClass(true)).not.toContain("flex-row");
  expect(codeActionsClass(false)).toContain("flex-row");
  expect(codeActionsClass(false)).toContain("justify-center");
});

test("the line under the field and under the boxes keeps one line's room", () => {
  expect(SAID_SLOT).toContain("min-h-5");
});

test("touch: the locked pages' scroll view ends at the keyboard's top (so the whole card scrolls above it); a pointer keeps it", () => {
  expect(authScrollStyle(true)).toEqual({ marginBottom: "env(keyboard-inset-height)" });
  expect(authScrollStyle(false)).toEqual({});
});
