//! Shared IMAP fetch + parse pipeline.
//!
//! Both App Password and Google OAuth2 authenticate, then hand off to this
//! module for the actual search → fetch → parse → confirm workflow.

use crate::db::{Database, SubmissionReceipt};
use crate::profile::{EmailAuthMethod, TaxpayerProfile};
use crate::receipt::parse_bir_receipt_email;

use super::auth_oauth::GoogleOAuthAuth;
use super::auth_password::AppPasswordAuth;

/// Trait that both auth backends implement.
pub trait ImapAuthenticator {
    fn authenticate(
        &self,
        client: imap::Client<native_tls::TlsStream<std::net::TcpStream>>,
    ) -> Result<
        (
            imap::Session<native_tls::TlsStream<std::net::TcpStream>>,
            Option<String>,
        ),
        anyhow::Error,
    >;

    /// The IMAP server hostname to connect to.
    fn host(&self) -> &str;
}

/// Fetch and process BIR confirmation emails for the given profile.
///
/// Automatically selects the correct auth strategy based on `email_auth_method`.
pub fn fetch_and_process_emails(
    profile: &TaxpayerProfile,
    db: &Database,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    if !profile.is_email_tracking_active() {
        return Ok(vec![]);
    }

    let email = profile.imap_email.as_deref().unwrap_or(&profile.email);

    // Build the right authenticator
    let (authenticator, host): (Box<dyn ImapAuthenticator>, String) = match profile
        .email_auth_method
    {
        EmailAuthMethod::AppPassword => {
            let host = profile
                .imap_host
                .as_deref()
                .unwrap_or("imap.gmail.com")
                .to_string();
            let auth =
                AppPasswordAuth::new(email, profile.imap_app_password.as_deref().unwrap_or(""))?;
            (Box::new(auth), host)
        }
        EmailAuthMethod::GoogleOAuth => {
            let access = profile.oauth_access_token.as_deref().unwrap_or("");
            let refresh = profile.oauth_refresh_token.as_deref().unwrap_or("");
            let auth = GoogleOAuthAuth::new(email, access, refresh)?;
            let host = "imap.gmail.com".to_string();
            (Box::new(auth), host)
        }
    };

    fetch_with_auth(authenticator.as_ref(), &host, db, profile)
}

/// Test that a connection can be established and authenticated.
/// Returns `Ok(())` on success or an error describing the failure.
pub fn test_connection(profile: &TaxpayerProfile) -> Result<Option<String>, anyhow::Error> {
    let email = profile.imap_email.as_deref().unwrap_or(&profile.email);

    let (authenticator, host): (Box<dyn ImapAuthenticator>, String) = match profile
        .email_auth_method
    {
        EmailAuthMethod::AppPassword => {
            let host = profile
                .imap_host
                .as_deref()
                .unwrap_or("imap.gmail.com")
                .to_string();
            let auth =
                AppPasswordAuth::new(email, profile.imap_app_password.as_deref().unwrap_or(""))?;
            (Box::new(auth), host)
        }
        EmailAuthMethod::GoogleOAuth => {
            let access = profile.oauth_access_token.as_deref().unwrap_or("");
            let refresh = profile.oauth_refresh_token.as_deref().unwrap_or("");
            let auth = GoogleOAuthAuth::new(email, access, refresh)?;
            let host = "imap.gmail.com".to_string();
            (Box::new(auth), host)
        }
    };

    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((&*host, 993_u16), &host, &tls)?;
    let (mut session, new_access_token) = authenticator.authenticate(client)?;
    session.select("INBOX")?;
    session.logout()?;
    Ok(new_access_token)
}

// ── Private ──────────────────────────────────────────────────────────────────

fn fetch_with_auth(
    auth: &dyn ImapAuthenticator,
    host: &str,
    db: &Database,
    profile: &TaxpayerProfile,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((host, 993_u16), host, &tls)?;
    let (mut session, new_access_token) = auth.authenticate(client)?;

    // Save the new access token if it was refreshed
    if let Some(token) = new_access_token {
        let mut updated_profile = profile.clone();
        updated_profile.oauth_access_token = Some(token);
        let _ = db.save_profile(updated_profile);
    }

    session.select("INBOX")?;

    // Search for unread BIR confirmation emails
    let seqs = session.search("UNSEEN FROM \"ebirforms-noreply@bir.gov.ph\"")?;

    let mut processed = Vec::new();

    for seq in seqs {
        let messages = session.fetch(seq.to_string(), "RFC822")?;
        for msg in messages.iter() {
            if let Some(body) = msg.body() {
                if let Ok(parsed_mail) = mailparse::parse_mail(body) {
                    let text_content = extract_text_body(&parsed_mail);

                    if let Ok(receipt) = parse_bir_receipt_email(&text_content) {
                        if let Ok(submission_receipt) = db.save_submission_receipt(&receipt) {
                            // Confirm the draft if we recognise the form type
                            if submission_receipt.form_type == "2551Qv2018"
                                || submission_receipt.form_type == "2551Q"
                            {
                                let _ = db.confirm_2551q_from_receipt(&submission_receipt);
                            }
                            processed.push(submission_receipt);

                            // Mark as seen so we don't re-process
                            let _ = session.store(seq.to_string(), "+FLAGS (\\Seen)");
                        }
                    }
                }
            }
        }
    }

    session.logout()?;
    Ok(processed)
}

/// Recursively extract text/plain content from a MIME message.
fn extract_text_body(mail: &mailparse::ParsedMail) -> String {
    let mut out = String::new();

    if mail.ctype.mimetype == "text/plain" {
        if let Ok(body) = mail.get_body() {
            out.push_str(&body);
        }
    }

    for sub in &mail.subparts {
        out.push_str(&extract_text_body(sub));
    }

    // Fallback: if no text/plain part found, try the root body
    if out.is_empty() {
        if let Ok(body) = mail.get_body() {
            out = body;
        }
    }

    out
}
