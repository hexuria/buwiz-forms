// The app's typeface: Inter, four static faces, registered before the first frame.
// Import this module once, for its side effect, from App.tsx (a bare `import "./theme/fonts"`).
// The same name is the theme's `font-sans` (nsh.json "theme" -> theme/theme.json), so every Text that sets
// no family of its own is drawn in Inter. Weights come from classes: font-normal 400, font-medium 500,
// font-semibold 600, font-bold 700 (the four faces below; any other weight snaps to the nearest).
import Inter from "./fonts/Inter-Regular.woff2";
import "./fonts/Inter-Medium.woff2";
import "./fonts/Inter-SemiBold.woff2";
import "./fonts/Inter-Bold.woff2";

/** The family name the files register under: "Inter". Use it only for a `style={{ fontFamily }}`; classes need no name. */
export const SANS: string = Inter;
