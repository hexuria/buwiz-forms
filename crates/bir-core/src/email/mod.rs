//! Email tracking module — fetches BIR confirmation emails via IMAP.
//!
//! Supports two authentication strategies:
//! - **App Password**: Standard IMAP `LOGIN` (works with any provider).
//! - **Google OAuth2**: IMAP `XOAUTH2` SASL (Gmail only, modern UX).

mod auth_oauth;
mod auth_password;
mod fetcher;
pub mod gmail_api;
mod inbox;
pub(crate) mod oauth_server;

/// How far back the inbox is searched for the BIR's confirmation emails (IMAP `SINCE`), and so how long after its send a
/// return counts as waiting for one: a return sent earlier could not be found by the search anyway. One number for both,
/// so "waiting" never claims more than the search can look at.
pub const BIR_EMAIL_WINDOW_DAYS: i64 = 30;

pub use auth_oauth::{
    GOOGLE_NOT_IN_THIS_VERSION, GoogleOAuthAuth, get_oauth_email, google_sign_in_available,
    start_oauth_flow,
};
pub use auth_password::AppPasswordAuth;
pub use fetcher::{
    ImapAuthenticator, fetch_and_process_emails, fetch_and_process_emails_for_address,
    fetch_and_process_emails_for_address_until, test_connection,
};
pub use inbox::{
    EmailPollPlan, plan_email_poll_for_address, resolve_inbox_fetch_profile,
    select_imap_auth_profile,
};
