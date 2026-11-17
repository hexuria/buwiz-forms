// The chrome layout for this window now (chrome_layout.ts over the platform and the window's width). Read inside JSX or a closure.
import { platform, windowSize } from "nsh";
import { isAvailable } from "nsh/desktop";
import { chromeLayout, MENU_ROW, TOOLBAR, type ChromeLayout } from "./chrome_layout";
import { globalMenu } from "./desktop/native";
import type { Os } from "./desktop/menu_model";

let globalAnswer: boolean | null = null;

/** Linux only: Plasma's Global Menu shows the app's menu (asked of Rust once). */
function globalMenuOnce(): boolean {
  if (platform.os !== "linux") return false;
  if (globalAnswer === null) {
    try {
      globalAnswer = globalMenu();
    } catch {
      globalAnswer = false;
    }
  }
  return globalAnswer;
}

/** A desktop with windows. */
export function isDesktop(): boolean {
  return isAvailable && platform.desktop;
}

/** The menu's OS (macOS, Windows, Linux), or null off the desktops. */
export function menuOs(): Os | null {
  if (!isDesktop()) return null;
  return platform.os === "macos" || platform.os === "windows" || platform.os === "linux" ? platform.os : null;
}

/** The chrome now (reactive on the window's width). */
export function chromeNow(): ChromeLayout {
  return chromeLayout({ os: platform.os, desktop: isDesktop(), globalMenu: globalMenuOnce(), width: windowSize().width });
}

/** How far the frame's chrome reaches down the window: the toolbar, and the menu row above it when there is one. */
export function chromeHeight(): number {
  return chromeNow().menuRow ? MENU_ROW + TOOLBAR : TOOLBAR;
}
