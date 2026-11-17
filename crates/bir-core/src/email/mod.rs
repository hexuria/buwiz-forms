//! Email tracking module — fetches BIR confirmation emails via IMAP.
//!
//! Supports two authentication strategies:
//! - **App Password**: Standard IMAP `LOGIN` (works with any provider).
//! - **Google OAuth2**: IMAP `XOAUTH2` SASL (Gmail only, modern UX).

mod auth_oauth;
mod auth_password;
mod fetcher;
mod inbox;
pub(crate) mod oauth_server;

pub use auth_oauth::{GoogleOAuthAuth, get_oauth_email, start_oauth_flow};
pub use auth_password::AppPasswordAuth;
pub use fetcher::{
    ImapAuthenticator, fetch_and_process_emails, fetch_and_process_emails_for_address,
    test_connection,
};
pub use inbox::{
    EmailPollPlan, plan_email_poll_for_address, resolve_inbox_fetch_profile,
    select_imap_auth_profile,
};
