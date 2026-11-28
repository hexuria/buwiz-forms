// sheetCap's arithmetic against the real numbers the Android emulator reports (#417's emulator: 411x914 dp, top inset
// 24, keyboard 336; signInLook.test.ts has the same numbers). The headless harness never raises a software keyboard, so
// this is a pure-function test, the signInLook.test.ts pattern. A tall sheet with the keyboard up must never extend
// under the keyboard or off the top: its top (the keyboard's top less the cap) must stay below the status bar.
import { expect, test } from "nsh/test";
import { sheetCap } from "./layers";

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
  // The 160 floor: a sheet never collapses to nothing, however tall the keyboard.
  expect(sheetCap(914, 24, 48, 800, true)).toBe(160);
});
