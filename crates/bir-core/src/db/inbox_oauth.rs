//! Google OAuth tokens keyed by inbox email.
//!
//! Several taxpayer profiles can share one Gmail inbox. Storing tokens on a
//! single `TaxpayerProfile` and then picking `list_profiles()` order made
//! reconnect appear successful while the confirmation poller kept the first
//! profile's revoked refresh token.

use rusqlite::{OptionalExtension, params};

use super::{Database, DbError, alert_kinds};
use crate::profile::{EmailAuthMethod, TaxpayerProfile, inbox_emails_match, normalize_inbox_email};

/// Tokens the IMAP poller uses for one shared inbox.
#[derive(Clone)]
pub struct InboxOAuthTokens {
    /// Original Google account email for the XOAUTH2 `user=` field.
    pub imap_user: String,
    pub access_token: String,
    pub refresh_token: String,
    pub updated_at: String,
}

impl InboxOAuthTokens {
    pub fn has_usable_refresh(&self) -> bool {
        !self.refresh_token.trim().is_empty()
    }
}

impl std::fmt::Debug for InboxOAuthTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboxOAuthTokens")
            .field("imap_user", &self.imap_user)
            .field("access_token", &"[redacted]")
            .field(
                "refresh_token",
                &if self.has_usable_refresh() {
                    "[present]"
                } else {
                    "[empty]"
                },
            )
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl Drop for InboxOAuthTokens {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.access_token.zeroize();
        self.refresh_token.zeroize();
    }
}

/// Result of writing a new Google OAuth grant for an inbox.
#[derive(Debug, Clone)]
pub struct InboxOAuthPersistResult {
    pub email: String,
    pub updated_tins: Vec<String>,
}

impl Database {
    /// Load the shared Google OAuth grant for this inbox, if any.
    pub fn inbox_oauth_tokens(
        &self,
        inbox_email: &str,
    ) -> Result<Option<InboxOAuthTokens>, DbError> {
        let key = normalize_inbox_email(inbox_email);
        if key.is_empty() {
            return Ok(None);
        }
        self.conn
            .query_row(
                "SELECT imap_user, access_token, refresh_token, updated_at
                 FROM inbox_oauth_tokens WHERE email = ?1",
                params![key],
                |row| {
                    Ok(InboxOAuthTokens {
                        imap_user: row.get(0)?,
                        access_token: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        refresh_token: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(DbError::from)
    }

    /// Persist a new Google OAuth grant for every profile using this inbox.
    ///
    /// Google omits `refresh_token` unless consent is re-granted. This app
    /// already sends `prompt=consent`. An empty refresh must not mark the
    /// inbox Connected while a dead refresh stays in place.
    pub fn persist_google_oauth_for_inbox(
        &self,
        google_email: &str,
        access_token: &str,
        refresh_token: &str,
        source_tin: Option<&str>,
    ) -> Result<InboxOAuthPersistResult, DbError> {
        let google_email = google_email.trim();
        let access_token = access_token.trim();
        let refresh_token = refresh_token.trim();
        if google_email.is_empty() {
            return Err(DbError::Other(
                "Google did not return an email address for this OAuth grant".to_string(),
            ));
        }
        if access_token.is_empty() {
            return Err(DbError::Other(
                "Google did not return an access token".to_string(),
            ));
        }
        if refresh_token.is_empty() {
            return Err(DbError::Other(
                "Google did not return a refresh token. Re-authorize with consent so a new refresh token is issued.".to_string(),
            ));
        }

        let key = normalize_inbox_email(google_email);
        let now = chrono::Utc::now().to_rfc3339();
        self.upsert_inbox_oauth_row(&key, google_email, access_token, refresh_token, &now)?;

        let mut updated_tins = Vec::new();
        for mut profile in self.list_profiles()? {
            let tin = profile.tin.full();
            let is_source = source_tin.is_some_and(|source| source == tin);
            let matches_inbox = inbox_emails_match(profile.inbox_email(), google_email);
            if !is_source && !matches_inbox {
                continue;
            }
            apply_oauth_grant_to_profile(
                &mut profile,
                google_email,
                access_token,
                refresh_token,
                is_source,
            );
            self.save_profile(profile)?;
            updated_tins.push(tin);
        }

        self.resolve_google_oauth_alerts_for_inbox(google_email)?;
        tracing::info!(
            inbox = %key,
            profile_count = updated_tins.len(),
            "Persisted Google OAuth tokens for a shared inbox"
        );
        Ok(InboxOAuthPersistResult {
            email: google_email.to_string(),
            updated_tins,
        })
    }

    /// Clear the shared grant so no sibling keeps a stale refresh token.
    pub fn disconnect_google_oauth_for_inbox(
        &self,
        inbox_email: &str,
    ) -> Result<Vec<String>, DbError> {
        let key = normalize_inbox_email(inbox_email);
        if !key.is_empty() {
            self.conn.execute(
                "DELETE FROM inbox_oauth_tokens WHERE email = ?1",
                params![key],
            )?;
        }

        let mut updated_tins = Vec::new();
        for mut profile in self.list_profiles()? {
            if !inbox_emails_match(profile.inbox_email(), inbox_email) {
                continue;
            }
            let tin = profile.tin.full();
            profile.oauth_access_token = None;
            profile.oauth_refresh_token = None;
            self.save_profile(profile)?;
            updated_tins.push(tin);
        }

        self.resolve_google_oauth_alerts_for_inbox(inbox_email)?;
        tracing::info!(
            inbox = %key,
            profile_count = updated_tins.len(),
            "Cleared Google OAuth tokens for a shared inbox"
        );
        Ok(updated_tins)
    }

    /// After a live token refresh, copy the new access token onto the inbox row
    /// and every sibling profile. Does not replace the refresh token.
    pub fn update_inbox_oauth_access_token(
        &self,
        inbox_email: &str,
        access_token: &str,
    ) -> Result<(), DbError> {
        let access_token = access_token.trim();
        if access_token.is_empty() {
            return Ok(());
        }
        let key = normalize_inbox_email(inbox_email);
        if !key.is_empty() {
            self.conn.execute(
                "UPDATE inbox_oauth_tokens SET access_token = ?1 WHERE email = ?2",
                params![access_token, key],
            )?;
        }
        for mut profile in self.list_profiles()? {
            if !inbox_emails_match(profile.inbox_email(), inbox_email) {
                continue;
            }
            profile.oauth_access_token = Some(access_token.to_string());
            self.save_profile(profile)?;
        }
        Ok(())
    }

    /// Reconnect (or a later successful poll) must clear the nag for every TIN
    /// that shares the inbox, not only the credential-owner profile.
    pub fn resolve_google_oauth_alerts_for_inbox(&self, inbox_email: &str) -> Result<(), DbError> {
        self.resolve_alert(None, alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED)?;
        for profile in self.list_profiles()? {
            if inbox_emails_match(profile.inbox_email(), inbox_email) {
                self.resolve_alert(
                    Some(&profile.tin.full()),
                    alert_kinds::GOOGLE_OAUTH_REFRESH_FAILED,
                )?;
            }
        }
        Ok(())
    }

    fn upsert_inbox_oauth_row(
        &self,
        email_key: &str,
        imap_user: &str,
        access_token: &str,
        refresh_token: &str,
        updated_at: &str,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO inbox_oauth_tokens (email, imap_user, access_token, refresh_token, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(email) DO UPDATE SET
                imap_user = excluded.imap_user,
                access_token = excluded.access_token,
                refresh_token = excluded.refresh_token,
                updated_at = excluded.updated_at",
            params![email_key, imap_user, access_token, refresh_token, updated_at],
        )?;
        Ok(())
    }
}

fn apply_oauth_grant_to_profile(
    profile: &mut TaxpayerProfile,
    google_email: &str,
    access_token: &str,
    refresh_token: &str,
    is_source: bool,
) {
    profile.oauth_access_token = Some(access_token.to_string());
    profile.oauth_refresh_token = Some(refresh_token.to_string());
    if is_source {
        profile.imap_email = Some(google_email.to_string());
        profile.email_auth_method = EmailAuthMethod::GoogleOAuth;
        profile.email_tracking_enabled = true;
    }
}
