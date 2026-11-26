// The Buwiz Forms logo on the pages before the app: the whole drawing with its words, never the emblem alone, never cut.
// One header for every one of them (the Buwiz sign-in, email and code pages, the PIN sign-in and Forgot PIN, first-run setup,
// the locked page), in the same place and at one size per screen class, so the logo changes in one place. The picture is a
// PNG `just icons` renders from assets/brand/buwiz-forms.svg (make_icons.py: 180 points high at 1x, 2x and 3x; NSH loads
// the one for the screen's density, and a phone shows it scaled down to 150).
import { Image, View, windowSize } from "nsh";
import { BREAK } from "../../theme/tokens";
import { TOUCH } from "../../ui";
import logo from "../../assets/brand/buwiz-forms-logo.png";

/** The id the logo carries on every page (the tests look for it). */
export const LOGO_ID = "brand-logo";

/**
 * The logo's height in points: 150 on a phone, 180 on a tablet (a touch screen at least 600 points on its short side) and on
 * a desktop. Its width follows the drawing's shape (1043 by 1134: a little narrower than tall).
 */
export function logoHeight(touch: boolean, size: { width: number; height: number }): number {
  if (!touch) return 180;
  return Math.min(size.width, size.height) >= BREAK.phone ? 180 : 150;
}

/** The whole logo, centred, at the screen's size. */
export function BrandLogo() {
  return (
    <View class="w-full items-center">
      <Image testID={LOGO_ID} accessibilityLabel="Buwiz Forms" src={logo} class="shrink-0 object-contain" style={{ height: logoHeight(TOUCH, windowSize()) }} />
    </View>
  );
}
