// The open pages' layout decisions, for touch and for a pointer (signInLook.ts) and the logo's size (brand.tsx). The headless
// test API reads no sizes or positions, so the numbers are checked here as the pages' record of them. The welcome line's
// "first time" is the host's answer now (welcome_first_time, accounts-and-lock W3): gate.test.tsx shows it once per account.
import { expect, test } from "nsh/test";
import {
  backRow, blockRoom, bottomPad, codeBoxWidth, keyboardCover, LOGO_STEPS_LARGE, LOGO_STEPS_PHONE, logoStep, logoSteps, openColumnMax, openSpacing, SAID_SLOT,
  signInHeights, underBackRowGuess,
} from "./signInLook";

test("the field, the buttons and the boxes: 46 points (48 on Android), the boxes 56 tall: whole touch targets", () => {
  expect(signInHeights(false)).toEqual({ field: 46, button: 46, box: 56 });
  expect(signInHeights(true)).toEqual({ field: 48, button: 48, box: 56 });
});

test("the code boxes are 46 wide where six fit, narrower on a 320-point phone, never under 36", () => {
  expect(codeBoxWidth(342)).toBe(46);
  expect(codeBoxWidth(272)).toBe(39);
  expect(codeBoxWidth(200)).toBe(36);
});

test("the logo's steps: a phone 150, 112, 80; a tablet and a computer 180, 128, 88, 64", () => {
  expect(LOGO_STEPS_PHONE).toEqual([150, 112, 80]);
  expect(LOGO_STEPS_LARGE).toEqual([180, 128, 88, 64]);
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

test("tablet and computer: the same boundaries for 180, 128, 88, 64", () => {
  const rest = 300;
  expect(logoStep(LOGO_STEPS_LARGE, 480, rest)).toEqual({ logo: 180, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 479, rest)).toEqual({ logo: 128, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 428, rest)).toEqual({ logo: 128, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 427, rest)).toEqual({ logo: 88, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 388, rest)).toEqual({ logo: 88, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 387, rest)).toEqual({ logo: 64, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 364, rest)).toEqual({ logo: 64, fits: true });
  expect(logoStep(LOGO_STEPS_LARGE, 363, rest)).toEqual({ logo: 64, fits: false });
});

test("the bottom safe area: the room ends above it, and the block's padding keeps clear of it unless the keyboard covers it", () => {
  // Android 360x640 dp with the three-button bar (top inset 24, bottom 48): 568 under the back row, 488 for the block.
  const under = underBackRowGuess(640, 24, true, true);
  expect(under).toBe(568);
  expect(blockRoom(under, 0, 48)).toBe(488);
  // The keyboard covers the bar: the larger of the two comes off, once.
  expect(blockRoom(under, 300, 48)).toBe(236);
  expect(bottomPad(16, 0, 48)).toBe(64);
  expect(bottomPad(16, 300, 48)).toBe(16);
  expect(bottomPad(8, 0, 0)).toBe(8);
});

test("tight spacing below 520 points of room: half the padding, half the logo's gap; decided from the room alone", () => {
  expect(openSpacing(528)).toEqual({ tight: false, pad: 16, logoGap: 24 });
  expect(openSpacing(520)).toEqual({ tight: false, pad: 16, logoGap: 24 });
  expect(openSpacing(519)).toEqual({ tight: true, pad: 8, logoGap: 12 });
  // A 360x640 Android phone with its three buttons draws tight, and gets 16 more points: 504 for the block.
  expect(openSpacing(blockRoom(568, 0, 48)).tight).toBe(true);
  expect(blockRoom(568, 0, 48, 8)).toBe(504);
  // The shortest desktop windows: 800x500 has 404, 420x480 has 384 for the block once tight.
  expect(openSpacing(blockRoom(underBackRowGuess(500, 0, false, false))).tight).toBe(true);
  expect(blockRoom(underBackRowGuess(500, 0, false, false), 0, 0, 8)).toBe(404);
  expect(blockRoom(underBackRowGuess(480, 0, false, false), 0, 0, 8)).toBe(384);
  // 1280x640 and a 375x667 phone keep the usual spacing.
  expect(openSpacing(blockRoom(underBackRowGuess(640, 0, false, false))).tight).toBe(false);
  expect(openSpacing(blockRoom(underBackRowGuess(667, 0, true, false))).tight).toBe(false);
});

test("the room the block has, before the page measures: a 375x667 phone, a 390x844 phone, a 1280x640 window", () => {
  // The window, less the top inset on touch (the 40-point title bar with a pointer) and the back row, less 16 above and below
  // the block.
  expect(blockRoom(underBackRowGuess(667, 0, true, false))).toBe(591);
  expect(blockRoom(underBackRowGuess(844, 47, true, false))).toBe(721);
  expect(blockRoom(underBackRowGuess(640, 0, false, false))).toBe(528);
  // A 390x844 phone with a 336-point keyboard up leaves 385 for the block: a 400-point page scrolls at the smallest step.
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

test("the keyboard: the room is what shows above it, counted once", () => {
  // iOS, a 390x844 phone (top inset 47) with a 300-point keyboard: the page is laid out under the keyboard, so all of it is
  // taken off, and a 300-point email page gets the 112 step and fits.
  const iosUnder = underBackRowGuess(844, 47, true, false);
  expect(iosUnder).toBe(753);
  expect(keyboardCover(300, iosUnder, iosUnder)).toBe(300);
  expect(blockRoom(iosUnder - 300)).toBe(421);
  expect(logoStep(LOGO_STEPS_PHONE, blockRoom(iosUnder - 300), 300)).toEqual({ logo: 112, fits: true });
  // Android (#417's emulator: 411x914 dp, top inset 24, keyboard 336): the window lays the content out above the keyboard, so
  // the page already measures 506 under the back row and nothing more is taken off; 150 still fits a 300-point page.
  const androidGuess = underBackRowGuess(914, 24, true, true);
  expect(androidGuess).toBe(842);
  expect(keyboardCover(336, androidGuess, androidGuess - 336)).toBe(0);
  expect(logoStep(LOGO_STEPS_PHONE, blockRoom(androidGuess - 336), 300)).toEqual({ logo: 150, fits: true });
  // An iOS host whose surface already ends at the keyboard (the page measures 417 under the back row): nothing more comes off,
  // and the same 300-point page gets the same 112 step as when the page took the keyboard off itself.
  expect(keyboardCover(300, iosUnder, iosUnder - 300)).toBe(0);
  expect(logoStep(LOGO_STEPS_PHONE, blockRoom(iosUnder - 300, keyboardCover(300, iosUnder, iosUnder - 300)), 300)).toEqual({ logo: 112, fits: true });
  // iPhone SE (375x667, top inset 20) with the number pad (216) on the code page: tight, and a 238-point page fits at 112.
  const se = underBackRowGuess(667, 20, true, false);
  expect(se).toBe(603);
  expect(openSpacing(blockRoom(se, 216)).tight).toBe(true);
  expect(logoStep(LOGO_STEPS_PHONE, blockRoom(se, 216, 0, 8), 238)).toEqual({ logo: 112, fits: true });
  // A window that resizes and reports no keyboard, and the keyboard down: nothing.
  expect(keyboardCover(0, 753, 417)).toBe(0);
  expect(keyboardCover(0, 753, 753)).toBe(0);
  // Part of it already taken (a note over the page): the rest.
  expect(keyboardCover(300, 753, 700)).toBe(247);
});
