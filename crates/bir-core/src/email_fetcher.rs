//! Legacy shim — delegates to the new `email` module.
//!
//! Kept for backward compatibility with callers that reference
//! `bir_core::email_fetcher::*` directly.

pub use crate::email::fetch_and_process_emails;
