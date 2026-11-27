// The Buwiz Forms logo on the pages before the app: the whole drawing with its words, never the emblem alone, never cut.
// One header for every one of them (the Buwiz sign-in, email and code pages, the PIN sign-in and Forgot PIN, first-run setup,
// the locked page), drawn by parts.tsx OpenPage at the step of its height the room allows (signInLook.ts logoStep), so the
// logo changes in one place. The picture is a PNG `just icons` renders from assets/brand/buwiz-forms.svg (make_icons.py: 180
// points high at 1x, 2x and 3x; NSH loads the one for the screen's density, and the smaller steps show it scaled down).
import { Image, View } from "nsh";
import logo from "../../assets/brand/buwiz-forms-logo.png";

/** The id the logo carries on every page (the tests look for it). */
export const LOGO_ID = "brand-logo";

/** The whole logo, centred, `height` points high; its width follows the drawing's shape (1043 by 1134). */
export function BrandLogo(props: { height: number }) {
  return (
    <View class="w-full items-center">
      <Image testID={LOGO_ID} accessibilityLabel="Buwiz Forms" src={logo} class="shrink-0 object-contain" style={{ height: props.height }} />
    </View>
  );
}
