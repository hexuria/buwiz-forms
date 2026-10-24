// When the filing chrome may show.
//
// The menu row and the window controls stay up for the whole life of the
// window. Everything that needs a finished account or profile — the toolbar
// (theme, notifications, search, new return), the taxpayer picker, the
// sidebar and its account switcher, and the filing menu commands — waits
// until first-run setup is done and the sign-in screen is not in front.
// The cloud account gate uses the same rule.

/**
 * @param {{
 *   booted: boolean,
 *   cloudLocked: boolean,
 *   appLocked: boolean,
 *   mode: string | null,
 *   setupPending: string | null,
 *   setupStep: string,
 * }} state
 * @returns {boolean}
 */
export function shellChromeVisible(state) {
  if (!state.booted || state.cloudLocked || state.appLocked) return false;
  // No mode yet: the "who will use this computer" choice, including the
  // first taxpayer typed for "Just me". The recovery key stays up after
  // the help-desk mode is saved, until it has been written down.
  if (!state.mode || state.setupPending) return false;
  if (state.setupStep === "helpdesk-key") return false;
  return true;
}
