//! When the filing chrome may show (chrome_gate.js). The menu row and the window controls stay up for the
//! whole life of the window. Everything that needs a finished account or profile waits until first-run
//! setup is done and the sign-in screen is not in front. The cloud account gate uses the same rule. Pure.

/// What the shell knows about its state.
#[derive(Debug, Clone)]
pub struct ChromeState {
    pub booted: bool,
    pub cloud_locked: bool,
    pub app_locked: bool,
    /// The "who will use this computer" choice; `None` until it is made.
    pub mode: Option<String>,
    /// The first-run step still to finish, or `None`.
    pub setup_pending: Option<String>,
    pub setup_step: String,
}

/// Whether the filing chrome may show in this state.
pub fn shell_chrome_visible(state: &ChromeState) -> bool {
    if !state.booted || state.cloud_locked || state.app_locked {
        return false;
    }
    if state.mode.is_none() || state.setup_pending.is_some() {
        return false;
    }
    if state.setup_step == "helpdesk-key" {
        return false;
    }
    true
}
