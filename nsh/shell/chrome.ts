// The chrome layout for this window now (chrome_layout.ts over the platform and the window's width). Read inside JSX or a closure.
import { computed, platform, windowSize } from "nsh";
import { isAvailable, Menu } from "nsh/desktop";
import { chromeLayout, MENU_ROW, TOOLBAR, type ChromeLayout } from "./chrome_layout";
import type { Os } from "./desktop/menu_model";

/** A desktop with windows. */
export function isDesktop(): boolean {
  return isAvailable && platform.desktop;
}

/** The menu's OS (macOS, Windows, Linux), or null off the desktops. */
export function menuOs(): Os | null {
  if (!isDesktop()) return null;
  return platform.os === "macos" || platform.os === "windows" || platform.os === "linux" ? platform.os : null;
}

/** The same rows and flags: a resize inside one breakpoint changes nothing. */
function sameLayout(a: ChromeLayout, b: ChromeLayout): boolean {
  return (Object.keys(a) as (keyof ChromeLayout)[]).every((k) => a[k] === b[k]);
}

// NSH says whether the menu really is in KDE Plasma's Global Menu: exported, and the window tied to it (Linux; false elsewhere).
// It turns false when the widget goes, so the layout falls back to the two rows while the app runs.
const layout = computed(
  () => chromeLayout({ os: platform.os, desktop: isDesktop(), globalMenu: platform.os === "linux" && Menu.inGlobalMenu(), width: windowSize().width }),
  { equals: sameLayout },
);

/** The chrome now (reactive; it changes when the layout does, not at every width). */
export function chromeNow(): ChromeLayout {
  return layout();
}

/** How far the frame's chrome reaches down the window: the toolbar, and the menu row above it when there is one. */
export function chromeHeight(): number {
  return chromeNow().menuRow ? MENU_ROW + TOOLBAR : TOOLBAR;
}
