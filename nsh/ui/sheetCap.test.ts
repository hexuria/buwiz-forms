// sheetCap's arithmetic against the real numbers the Android emulator reports (#417's emulator: 411x914 dp, top inset
// 24, keyboard 336; signInLook.test.ts has the same numbers). The headless harness never raises a software keyboard, so
// this is a pure-function test, the signInLook.test.ts pattern. A tall sheet with the keyboard up must never extend
// under the keyboard or off the top: its top (the keyboard's top less the cap) must stay below the status bar.
import { expect, test } from "nsh/test";
import { sheetBodyRoomOf, sheetBottomPad, sheetCap } from "./layers";
import { T_PX } from "./look";

/** The sheet's top edge when it is as tall as the cap, bottom-aligned at the keyboard's top. */
function sheetTop(windowHeight: number, keyboardHeight: number, cap: number): number {
  return windowHeight - keyboardHeight - cap;
}

test("a tall sheet with the keyboard up never extends under the keyboard or off the top", () => {
  // Android, 3-button navigation: the navigation bar (48) is behind the keyboard, so the keyboard inset is 336 + 48 = 384.
  // The room above the keyboard and below the status bar is (914 - 384) - 24 = 506.
  const cap3 = sheetCap(914, 24, 48, 384, true);
  expect(sheetTop(914, 384, cap3)).toBeGreaterThanOrEqual(24);
  // Android, gesture navigation: the strip (16) behind the keyboard, the inset 352. The room is (914 - 352) - 24 = 538.
  const capG = sheetCap(914, 24, 16, 352, true);
  expect(sheetTop(914, 352, capG)).toBeGreaterThanOrEqual(24);
  // Android, keyboard down: no keyboard comes off in either mode.
  expect(sheetCap(914, 24, 48, 0, true)).toBe(914 - 24 - 24);
  // iOS (390x844, top inset 47, home strip 34, keyboard 300 over it): unchanged — the keyboard sits above the strip and
  // only the height over it counts.
  expect(sheetCap(844, 47, 34, 300, false)).toBe(844 - 47 - (300 - 34) - 24);
  // The 160 floor holds while the room allows it (portrait, a normal keyboard).
  expect(sheetCap(914, 24, 48, 384, true)).toBe(914 - 24 - 384 - 24);
  // Landscape or split screen (360 tall, keyboard 200): the room is 360 - 24 - 200 = 136, and the floor is
  // clamped to it — never past it, so the sheet's top is never off the screen.
  const short = sheetCap(360, 24, 0, 200, true);
  expect(short).toBe(136);
  expect(sheetTop(360, 200, short)).toBeGreaterThanOrEqual(24);
  // API 29 and below: the keyboard inset is never reported, so the cap is the no-keyboard room — exactly the
  // formula before it was keyboard-aware (never worse than main's).
  const withoutKeyboard = sheetCap(914, 24, 48, 0, true);
  expect(withoutKeyboard).toBe(914 - 24 - 24);
  const mainFormula = Math.max(160, 914 - 24 - Math.max(0, 0 - 48) - 24);
  expect(withoutKeyboard).toBe(mainFormula);
  // Nothing left above the keyboard at all: the cap is 0 rather than negative or off the top.
  expect(sheetCap(360, 24, 0, 336, true)).toBe(0);
});

// #515's emulator proof (1080x2400 at 2.625: a window 914.29 tall, status bar 24, keyboard 336.38 in both navigation modes,
// since it reaches the screen's bottom over the navigation bar; the bottom safe area 48 with 3-button navigation, 24 with
// gestures). The sheet's body and its list padded for the navigation bar again above the keyboard: the list was 24 shorter
// with 3-button navigation than with gestures, and the last row was cut in both (27.8 hidden with 3-button, 3.8 with gestures).
const W = 2400 / 2.625;
const KB = 883 / 2.625;
/** The pick list's height for a sheet body this tall: Pick.tsx's listHeight. */
const pickList = (room: number) => Math.max(T_PX * 2, room - 32 - 12 - T_PX);

test("with the keyboard up on Android a sheet does not pad for the navigation bar the keyboard covers", () => {
  // The pad: none with the keyboard up, whichever navigation; the whole inset with it down.
  expect(sheetBottomPad(48, KB, true)).toBe(0);
  expect(sheetBottomPad(24, KB, true)).toBe(0);
  expect(sheetBottomPad(48, 0, true)).toBe(48);
  expect(sheetBottomPad(24, 0, true)).toBe(24);
  // The panel's cap does not depend on the navigation (sheetCap takes the whole keyboard on Android).
  const cap3 = Math.min(W * 0.9, sheetCap(W, 24, 48, KB, true));
  const capG = Math.min(W * 0.9, sheetCap(W, 24, 24, KB, true));
  expect(cap3).toBe(capG);
  // So the body, and the list in it, are the same height in both navigation modes: the 24-point difference the proof measured
  // was the navigation bar counted twice.
  const room3 = sheetBodyRoomOf(cap3, 48, KB, true, true);
  const roomG = sheetBodyRoomOf(capG, 24, KB, true, true);
  expect(room3).toBe(roomG);
  expect(pickList(room3)).toBe(pickList(roomG));
  // And each grew by what it lost: 48 with 3-button navigation, 24 with gestures. The proof's last row hid 27.8 and 3.8 of
  // itself below the list's end, so with the room back it is whole in both.
  const header = T_PX + 16 + 1;
  const grew3 = room3 - (cap3 - 12 - header - 48);
  const grewG = roomG - (capG - 12 - header - 24);
  expect(Math.abs(grew3 - 48)).toBeLessThan(1e-9);
  expect(Math.abs(grewG - 24)).toBeLessThan(1e-9);
  expect(grew3).toBeGreaterThan(27.8);
  expect(grewG).toBeGreaterThan(3.8);
  // With the keyboard down nothing changes on Android: the body still pads by the whole inset.
  const down = Math.min(W * 0.9, sheetCap(W, 24, 48, 0, true));
  expect(sheetBodyRoomOf(down, 48, 0, true, true)).toBe(down - 12 - header - 48);
});

test("iOS and the desktop keep today's sheet body exactly", () => {
  const header = T_PX + 16 + 1;
  // iOS (390x844, status bar 47, home strip 34, keyboard 300 over the strip): the strip is still padded, keyboard up or down.
  expect(sheetBottomPad(34, 300, false)).toBe(34);
  const iosCap = Math.min(844 * 0.9, sheetCap(844, 47, 34, 300, false));
  expect(sheetBodyRoomOf(iosCap, 34, 300, false, true)).toBe(iosCap - 12 - header - 34);
  const iosDown = Math.min(844 * 0.9, sheetCap(844, 47, 34, 0, false));
  expect(sheetBodyRoomOf(iosDown, 34, 0, false, true)).toBe(iosDown - 12 - header - 34);
  // A desktop window in the phone layout (no insets, no software keyboard, no touch header).
  expect(sheetBottomPad(0, 0, false)).toBe(0);
  const deskCap = Math.min(800 * 0.9, sheetCap(800, 0, 0, 0, false));
  expect(sheetBodyRoomOf(deskCap, 0, 0, false, false)).toBe(deskCap - 12 - (28 + 16 + 1) - 0);
});
