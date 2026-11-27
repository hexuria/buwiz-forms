// The open pages' layout decisions (the Buwiz sign-in's first, email and code pages, and the PIN sign-in, setup and locked
// pages on the same frame: parts.tsx OpenPage), pure so a test reads them for touch and for a pointer. The owner's layout C:
// a fixed back row at the top, and under it one block (the logo, the page, its button and links) centred in the room left,
// the logo stepping down in size before anything would scroll.

/**
 * The sign-in pages' own heights, in points: the field and the buttons 46 (48 on Android, its touch target), the code boxes 56
 * tall. Every one is a whole touch target (44 points on iOS, 48 dp on Android).
 */
export function signInHeights(android: boolean): { field: number; button: number; box: number } {
  const control = android ? 48 : 46;
  return { field: control, button: control, box: 56 };
}

/** The code boxes: 6 apart, 8 more after the third (two groups of three, 14 apart). */
export const BOX_GAP = 6;
export const BOX_GROUP_GAP = BOX_GAP + 8;

/**
 * A code box's width for the column it sits in: 46 points, narrower on a phone too narrow for six of them (a 320-point
 * screen), never under 36.
 */
export function codeBoxWidth(column: number, length = 6): number {
  const gaps = (length - 2) * BOX_GAP + BOX_GROUP_GAP;
  return Math.max(36, Math.min(46, Math.floor((column - gaps) / length)));
}

/** The open pages' column: at most 340 points wide with a pointer, 400 on touch (a tablet keeps a phone's column). */
export function openColumnMax(touch: boolean): number {
  return touch ? 400 : 340;
}

/** The open pages' sides: 24 points each. */
export const OPEN_SIDE = 24;

/** The room kept over and under the page's block inside its scroll view, so it never touches the back row or the bottom. */
export const BLOCK_PAD_Y = 16;

/** Between the logo and what is under it. */
export const LOGO_GAP = 24;

/** Under this room (at the usual padding) the page draws tight: its gaps step down with the logo (a short desktop window). */
export const TIGHT_ROOM = 520;

/**
 * The page's spacing for the room it has at the usual padding (blockRoom with BLOCK_PAD_Y): the usual, or tight when the room is
 * under TIGHT_ROOM: 8 over and under the block instead of 16, 12 under the logo instead of 24, and the page's own gaps a step
 * smaller (parts.tsx). Decided from the room alone, never from what the block measures, so drawing tight can never undo itself.
 */
export function openSpacing(usualRoom: number): { tight: boolean; pad: number; logoGap: number } {
  const tight = usualRoom < TIGHT_ROOM;
  return tight ? { tight, pad: 8, logoGap: 12 } : { tight, pad: BLOCK_PAD_Y, logoGap: LOGO_GAP };
}

/** The block's padding under it: its own, and the bottom safe area (a home indicator, Android's buttons) unless the keyboard covers it. */
export function bottomPad(pad: number, cover: number, safeBottom: number): number {
  return pad + (cover > 0 ? 0 : safeBottom);
}

/**
 * The logo's heights, largest first (the owner's layout C): a phone (a touch screen under 600 points on its short side) 150,
 * 112, 80; a tablet and a computer 180, 128, 88, and 64 for the shortest desktop windows (420 by 480).
 */
export const LOGO_STEPS_PHONE: readonly number[] = [150, 112, 80];
export const LOGO_STEPS_LARGE: readonly number[] = [180, 128, 88, 64];

/** The logo's steps for this screen. */
export function logoSteps(touch: boolean, window: { width: number; height: number }): readonly number[] {
  return touch && Math.min(window.width, window.height) < 600 ? LOGO_STEPS_PHONE : LOGO_STEPS_LARGE;
}

/**
 * The logo's height for the room the page has: the largest step at which the whole block (the logo and `rest`, everything under
 * it with the gap above it) fits `room` (the height under the back row, less the block's own padding). When even the smallest
 * does not fit (a phone's keyboard up, a very short window), the smallest, and `fits` false: the page scrolls.
 */
export function logoStep(steps: readonly number[], room: number, rest: number): { logo: number; fits: boolean } {
  for (const logo of steps) {
    if (logo + rest <= room) return { logo, fits: true };
  }
  return { logo: steps[steps.length - 1], fits: false };
}

/**
 * The room for the block: the height under the back row (what the page measures, else what the window gives), less what covers
 * its bottom (the keyboard's part, keyboardCover, or else the bottom safe area: a home indicator, Android's three buttons), less
 * the block's padding over and under it.
 */
export function blockRoom(underBackRow: number, cover = 0, safeBottom = 0, pad = BLOCK_PAD_Y): number {
  return Math.max(0, underBackRow - Math.max(cover, safeBottom) - 2 * pad);
}

/**
 * The back row: a fixed row at the top of the page, under the window's title bar (never in it) and inside the safe area, the
 * same on every page before the app, with or without a back control in it. Its height is a touch target on touch (44 points on
 * iOS, a navigation bar's; 48 dp on Android), 40 with a pointer; the control sits `left` points in from the page's left edge.
 */
export function backRow(touch: boolean, android: boolean, safeLeft: number): { height: number; left: number } {
  return { height: backRowHeight(touch, android), left: safeLeft + (touch ? 8 : 12) };
}

/** The back row's height (backRow). */
export function backRowHeight(touch: boolean, android: boolean): number {
  if (!touch) return 40;
  return android ? 48 : 44;
}

/**
 * Before the page has measured itself: the height under the back row as the window gives it (the window, less the locked
 * frame's top: its 40-point title bar with a pointer, only the top inset on touch; less the back row).
 */
export function underBackRowGuess(windowHeight: number, safeTop: number, touch: boolean, android: boolean): number {
  return windowHeight - (touch ? 0 : 40) - safeTop - backRowHeight(touch, android);
}

/**
 * The room kept for one sentence under the field and under the boxes (12-point text on the kit's 1.618 line: 19.4 points), so a
 * sentence appearing or going moves nothing. `min-h-5` is 20 points.
 */
export const SAID_SLOT = "min-h-5 w-full";

/**
 * How much of the software keyboard the open page must still keep itself clear of: the keyboard's height over the surface
 * (`keyboardFrame().height`, the bottom safe area under it included), less what the platform already took off the page. One
 * rule, from what the page measures against what the window gives (`guess`, underBackRowGuess, from the page's own top):
 * - iOS (pin 8c78cfd6): the surface keeps its full height under the keyboard (surface.rs `keyboard_changed` only reports it);
 *   the kit gives the focused field's scroll view a bottom content inset instead (mount.rs `avoid_keyboard`). The page
 *   measures its full height: all of the keyboard comes off.
 * - Android at the pin: the window lays the app's content out above the keyboard while `windowSize()` and the keyboard's height
 *   stay the full screen's (the "double lift" of #410). The page already measures the shorter height: none comes off again.
 * - A host that resizes its window reports no keyboard: none.
 * The page pads its block by this (parts.tsx OpenPage); it never shrinks its scroll view, whose keyboard inset is the kit's.
 */
export function keyboardCover(keyboard: number, guess: number, measured: number): number {
  return Math.max(0, keyboard - Math.max(0, guess - measured));
}
