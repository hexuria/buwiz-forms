// The open pages' layout decisions, for touch and for a pointer (signInLook.ts), the logo's size (brand.tsx) and the welcome
// line's "first time" (welcome.tsx). The headless test API reads no sizes or positions, so the numbers are checked here as the
// pages' record of them.
import { expect, test } from "nsh/test";
import {
  authScrollStyle, backRow, blockRoom, codeBoxWidth, LOGO_STEPS_LARGE, LOGO_STEPS_PHONE, logoStep, logoSteps, openColumnMax, SAID_SLOT,
  signInHeights, underBackRowGuess,
} from "./signInLook";
import { accountKey, welcomedAfter } from "./welcome";

test("the field, the buttons and the boxes: 46 points (48 on Android), the boxes 56 tall: whole touch targets", () => {
  expect(signInHeights(false)).toEqual({ field: 46, button: 46, box: 56 });
  expect(signInHeights(true)).toEqual({ field: 48, button: 48, box: 56 });
});

test("the code boxes are 46 wide where six fit, narrower on a 320-point phone, never under 36", () => {
  expect(codeBoxWidth(342)).toBe(46);
  expect(codeBoxWidth(272)).toBe(39);
  expect(codeBoxWidth(200)).toBe(36);
});

test("the logo's steps: a phone 150, 112, 80; a tablet and a computer 180, 128, 88", () => {
  expect(LOGO_STEPS_PHONE).toEqual([150, 112, 80]);
  expect(LOGO_STEPS_LARGE).toEqual([180, 128, 88]);
  expect(logoSteps(true, { width: 375, height: 667 })).toBe(LOGO_STEPS_PHONE);
  expect(logoSteps(true, { width: 844, height: 390 })).toBe(LOGO_STEPS_PHONE);
  expect(logoSteps(true, { width: 820, height: 1180 })).toBe(LOGO_STEPS_LARGE);
  expect(logoSteps(false, { width: 390, height: 844 })).toBe(LOGO_STEPS_LARGE);
  expect(logoSteps(false, { width: 1280, height: 640 })).toBe(LOGO_STEPS_LARGE);
});

test("phone: the largest step at which the whole block fits, at each boundary; the smallest and a scroll past the last", () => {
  const rest = 400;
  expect(logoStep(LOGO_STEPS_PHONE, 550, rest)).toEqual({ logo: 150, fits: true });
  expect(logoStep(LOGO_STEPS_PHONE, 549, rest)).toEqual({ logo: 112, fits: true });
  expect(logoStep(LOGO_STEPS_PHONE, 512, rest)).toEqual({ logo: 112, fits: true });
  expect(logoStep(LOGO_STEPS_PHONE, 511, rest)).toEqual({ logo: 80, fits: true });
  expect(logoStep(LOGO_STEPS_PHONE, 480, rest)).toEqual({ logo: 80, fits: true });
  expect(logoStep(LOGO_STEPS_PHONE, 479, rest)).toEqual({ logo: 80, fits: false });
});

test("tablet and computer: the same boundaries for 180, 128, 88", () => {
  const rest = 300;
  expect(logoStep(LOGO_STEPS_LARGE, 480, rest)).toEqual({ logo: 180, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 479, rest)).toEqual({ logo: 128, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 428, rest)).toEqual({ logo: 128, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 427, rest)).toEqual({ logo: 88, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 388, rest)).toEqual({ logo: 88, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 387, rest)).toEqual({ logo: 88, fits: false });
});

test("the room the block has, before the page measures: a 375x667 phone, a 390x844 phone, a 1280x640 window", () => {
  // The window, less the 40-point title bar, the top inset and the back row, less 16 above and below the block.
  expect(blockRoom(underBackRowGuess(667, 0, true, false))).toBe(551);
  expect(blockRoom(underBackRowGuess(844, 47, true, false))).toBe(681);
  expect(blockRoom(underBackRowGuess(640, 0, false, false))).toBe(528);
  // A 390x844 phone with a 336-point keyboard up leaves 461 for the block: a 400-point page scrolls at the smallest step.
  expect(logoStep(LOGO_STEPS_PHONE, blockRoom(underBackRowGuess(844 - 336, 47, true, false)), 400)).toEqual({ logo: 80, fits: false });
  expect(blockRoom(10)).toBe(0);
});

test("the back row: under the title bar, at the top left inside the safe area, a touch target on touch", () => {
  expect(backRow(false, false, 0)).toEqual({ height: 40, left: 12 });
  expect(backRow(true, false, 0)).toEqual({ height: 44, left: 8 });
  expect(backRow(true, true, 0)).toEqual({ height: 48, left: 8 });
  expect(backRow(true, false, 47)).toEqual({ height: 44, left: 55 });
  expect(openColumnMax(false)).toBe(340);
  expect(openColumnMax(true)).toBe(400);
});

test("the line under the field and under the boxes keeps one line's room", () => {
  expect(SAID_SLOT).toContain("min-h-5");
});

test("touch: the open pages' scroll view ends at the keyboard's top (so the whole page scrolls above it); a pointer keeps it", () => {
  expect(authScrollStyle(true)).toEqual({ marginBottom: "env(keyboard-inset-height)" });
  expect(authScrollStyle(false)).toEqual({});
});

test("welcome: the first sign-in of an account here is new and written down; the second is not", () => {
  expect(accountKey({ user_id: "u-1", email: "Juana@Example.com" })).toBe("u-1");
  expect(accountKey({ user_id: null, email: " Juana@Example.com " })).toBe("juana@example.com");
  expect(accountKey({ user_id: null, email: null })).toBe(null);
  const first = welcomedAfter(null, "u-1");
  expect(first).toEqual({ first: true, next: '["u-1"]' });
  expect(welcomedAfter(first.next, "u-1").first).toBe(false);
  expect(welcomedAfter(first.next, "u-2")).toEqual({ first: true, next: '["u-1","u-2"]' });
  expect(welcomedAfter("not json", "u-1")).toEqual({ first: true, next: '["u-1"]' });
});
