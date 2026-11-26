// The open pages' layout decisions, for touch and for a pointer (signInLook.ts), the logo's size (brand.tsx) and the welcome
// line's "first time" (welcome.tsx). The headless test API reads no sizes or positions, so the numbers are checked here as the
// pages' record of them.
import { expect, test } from "nsh/test";
import { logoHeight } from "./brand";
import { authScrollStyle, backRowHeight, codeBoxWidth, firstPageBottom, openColumnMax, openTop, SAID_SLOT, signInHeights } from "./signInLook";
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

test("the logo: 150 points high on a phone, 180 on a tablet and a desktop", () => {
  expect(logoHeight(true, { width: 390, height: 844 })).toBe(150);
  expect(logoHeight(true, { width: 844, height: 390 })).toBe(150);
  expect(logoHeight(true, { width: 820, height: 1180 })).toBe(180);
  expect(logoHeight(false, { width: 390, height: 844 })).toBe(180);
  expect(logoHeight(false, { width: 1440, height: 900 })).toBe(180);
});

test("the logo's place: the column starts from the window alone, under a back row that every page keeps", () => {
  expect(openTop(true, 844)).toBe(12);
  expect(openTop(false, 900)).toBe(120);
  expect(openTop(false, 500)).toBe(24);
  expect(backRowHeight(false, false)).toBe(40);
  expect(backRowHeight(true, false)).toBe(44);
  expect(backRowHeight(true, true)).toBe(48);
  expect(openColumnMax(false)).toBe(340);
  expect(openColumnMax(true)).toBe(400);
  expect(firstPageBottom(0)).toBe(42);
  expect(firstPageBottom(34)).toBe(50);
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
