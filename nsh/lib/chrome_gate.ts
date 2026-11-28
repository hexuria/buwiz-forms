// When the filing chrome may show (chrome_gate.js). The menu row and the window controls stay up for the whole life of the
// window. Everything that needs a finished account or profile (the toolbar, the taxpayer picker, the sidebar and its
// account switcher, and the filing menu commands) waits until first-run setup is done and the sign-in screen is not
// in front. The cloud account gate uses the same rule. Pure.

/** What the shell knows about its state. */
export interface ChromeState {
  booted: boolean;
  cloudLocked: boolean;
  appLocked: boolean;
  /** The "who will use this computer" choice; null until it is made. */
  mode: string | null;
  /** The first-run step still to finish, or null. */
  setupPending: string | null;
  setupStep: string;
}

/** Whether the filing chrome may show in this state. */
export function shellChromeVisible(state: ChromeState): boolean {
  if (!state.booted || state.cloudLocked || state.appLocked) return false;
  // No mode yet: the "who will use this computer" choice, including the first taxpayer typed for "Just me". The recovery
  // key stays up after the help-desk mode is saved, until it has been written down; the PIN page does too when Back returns
  // there from the key (before the mode is saved, `!state.mode` already hides the chrome).
  if (!state.mode || state.setupPending) return false;
  if (state.setupStep === "helpdesk-key" || state.setupStep === "helpdesk-pin") return false;
  return true;
}
