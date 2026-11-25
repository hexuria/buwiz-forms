// Room at the end of a page's scroll for the on-screen keyboard, so everything above it can be scrolled into view.
//
// A phone's keyboard covers the bottom of the window and the window does not shrink for it (the app draws edge to edge):
// a page's last fields and its buttons sat under it with nothing to scroll them out from (seen on Android, New taxpayer and
// Choose forms). Each page-level Scroll ends with this spacer: as tall as the keyboard reaches into the surface while it is
// up (`keyboardFrame`, the kit's way to keep content clear of it, website/docs/forms-and-keyboard.md), nothing otherwise, so
// a desktop, or a tablet with a hardware keyboard, lays out exactly as before.
//
// Android only. On iOS the kit already keeps that room itself: with the spacer an iPhone page ended in one keyboard height
// of blank space too many (New taxpayer 74 → 409 pt, Choose forms 77 → 393 pt, ios-submission's measurement at 36505288,
// try/ios-keyboard-pad-proof e077da93), and without it every field and button was already reachable.
import { keyboardFrame, platform, View } from "nsh";

export function KeyboardRoom() {
  return (
    <If when={platform.os === "android" && keyboardFrame().height > 0}>
      <View testID="keyboard-room" class="shrink-0" style={{ height: keyboardFrame().height }} />
    </If>
  );
}
