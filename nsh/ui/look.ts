// The kit's shared look decisions: density by input (pointer or touch) and the tone names the old app used
// (fg, fg2, fg3, ...) mapped onto theme token classes. Only the kit reads these; screens pass tones by name.
import { platform } from "nsh";

/**
 * Touch targets: true where the platform has no pointer as its main input (`platform.desktop` false: iOS, Android,
 * phone and tablet browsers; nsh .nsh/types/nsh/index.d.ts PlatformInfo.desktop). Controls are then at least 44 points
 * tall; with a pointer they are 32 (28 dense). Fixed while the app runs.
 */
export const TOUCH = !platform.desktop;

/** A text or icon tone, by the old app's palette keys. */
export type Tone = "fg" | "fg2" | "fg3" | "primaryFg" | "accent" | "success" | "warning" | "danger";

/** Text color classes per tone. */
export const TEXT_TONE: Record<Tone, string> = {
  fg: "text-foreground",
  fg2: "text-muted-foreground",
  fg3: "text-subtle-foreground",
  primaryFg: "text-primary-foreground",
  accent: "text-accent-blue",
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
};

/** Icon tint classes per tone (an SVG's strokes take `tintColor`). */
export const TINT_TONE: Record<Tone, string> = {
  fg: "tint-foreground",
  fg2: "tint-muted-foreground",
  fg3: "tint-subtle-foreground",
  primaryFg: "tint-primary-foreground",
  accent: "tint-accent-blue",
  success: "tint-success",
  warning: "tint-warning",
  danger: "tint-danger",
};

/**
 * The old app's line height for every size of text: gpui's default, the golden ratio (1.618) times the font size. The kit's Text sets
 * it, so a 20-point title takes 32.4 points of line, a 13-point body 21, a 12-point meta 19.4, as the old screens did; the type
 * classes' own ratios (nsh.json: 1.4 to 1.5) draw every block of text 4 to 8 percent shorter than the old app's. A `leading-*` class
 * given to a Text replaces it (the library card title fixes 18 points).
 */
export const LEADING = "leading-[1.618]";

/** The keyboard focus ring of a pressable control: two points of the accent outside it. */
export const FOCUS_RING = "focus-visible:ring-2 focus-visible:ring-accent-blue";

/** A control's height: 32 with a pointer, 44 on touch. */
export const CTL = TOUCH ? "min-h-11" : "h-8";
/** A dense control's height: 28 with a pointer, 44 on touch. */
export const CTL_SM = TOUCH ? "min-h-11" : "h-7";

/** Extra reach around small targets on touch (a 16-point box answers a finger 14 points away), none with a pointer. */
export const SMALL_HIT = TOUCH ? 14 : 0;

/** Joins class strings, skipping empty parts. */
export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}
