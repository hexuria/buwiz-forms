// The open pages' layout decisions (the Buwiz sign-in's first, email and code pages, and the PIN sign-in, setup and locked
// pages on the same frame: parts.tsx OpenPage), pure so a test reads them for touch and for a pointer.

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

/**
 * Where the column starts, so the logo is in the same place on every page: 12 points under the title bar on touch; with a
 * pointer, from the window's height alone (the page's own height never moves it), as if a 620-point page were centred.
 */
export function openTop(touch: boolean, windowHeight: number): number {
  if (touch) return 12;
  return Math.max(24, Math.round((windowHeight - 40 - 620) / 2));
}

/** The room over the logo for the back control (on every page, so the logo never moves): 40 points, a touch target on touch. */
export function backRowHeight(touch: boolean, android: boolean): number {
  if (!touch) return 40;
  return android ? 48 : 44;
}

/** The first page's button sits this far above the screen's bottom on touch (above the home indicator, which it clears). */
export function firstPageBottom(safeBottom: number): number {
  return Math.max(42, safeBottom + 16);
}

/**
 * The room kept for one sentence under the field and under the boxes (12-point text on the kit's 1.618 line: 19.4 points), so a
 * sentence appearing or going moves nothing. `min-h-5` is 20 points.
 */
export const SAID_SLOT = "min-h-5 w-full";

/**
 * The open pages' scroll view on touch ends at the keyboard's top: a bottom margin as tall as the keyboard
 * (`env(keyboard-inset-height)`, 0 while it is down, moving with it). Without it the only room to scroll is what the platform
 * makes for the focused field (the field, not the button under it): "Continue" stayed under the keyboard and could not be
 * scrolled out. With it the whole page is in a viewport above the keyboard and scrolls in it. Nothing changes while the
 * keyboard is down, nor with a pointer.
 */
export function authScrollStyle(touch: boolean): { marginBottom?: "env(keyboard-inset-height)" } {
  return touch ? { marginBottom: "env(keyboard-inset-height)" } : {};
}
