// The sign-in cards' layout decisions (signInCards.tsx), pure so a test reads them for touch and for a pointer. The heights
// themselves are the kit's: a control is 32 points with a pointer and 44 on touch (nsh/ui look.ts CTL, used by AppButton and
// TextField), a code box 32 by 32 with a pointer and 44 wide by 48 tall on touch (nsh/ui fields.tsx PinCells).

/** The kit's heights, in points, as the sign-in cards get them (for the record and its test; the kit's classes draw them). */
export function signInHeights(touch: boolean): { field: number; button: number; box: number } {
  return touch ? { field: 44, button: 44, box: 48 } : { field: 32, button: 32, box: 32 };
}

/**
 * The room kept for one sentence under the field and under the boxes (12-point text on the kit's 1.618 line: 19.4 points), so a
 * sentence appearing or going moves nothing. `min-h-5` is 20 points.
 */
export const SAID_SLOT = "min-h-5 w-full";

/**
 * How the code page's two secondary actions sit: stacked at full width on touch ("Resend…" first, then "Change email", 12
 * points apart), a centred row with a pointer.
 */
export function codeActionsClass(touch: boolean): string {
  return touch ? "w-full gap-3" : "w-full flex-row flex-wrap justify-center gap-3";
}

/**
 * The locked pages' scroll view (parts.tsx AuthCard) on touch ends at the keyboard's top: a bottom margin as tall as the
 * keyboard (`env(keyboard-inset-height)`, 0 while it is down, moving with it). The page is a little taller than a phone held
 * sideways, so without it the only room to scroll is what the platform makes for the focused field (the field, not the button
 * under it): "Continue with email" stayed under the keyboard and could not be scrolled out. With it the whole card is in a
 * viewport above the keyboard and scrolls in it. Nothing changes while the keyboard is down, nor with a pointer.
 */
export function authScrollStyle(touch: boolean): { marginBottom?: "env(keyboard-inset-height)" } {
  return touch ? { marginBottom: "env(keyboard-inset-height)" } : {};
}
