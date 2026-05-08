//! Shared IMAP fetch + parse pipeline.
//!
//! Both App Password and Google OAuth2 authenticate, then hand off to this
//! module for the actual search → fetch → parse → confirm workflow.

use crate::db::{Database, SubmissionReceipt};
use crate::profile::{EmailAuthMethod, TaxpayerProfile};
use crate::receipt::parse_bir_receipt_email;
use chrono::Datelike;

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
    db: std::sync::Arc<std::sync::Mutex<Database>>,
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
            let auth = GoogleOAuthAuth::new(&profile.email, access, refresh)?;
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
            let auth = GoogleOAuthAuth::new(&profile.email, access, refresh)?;
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
    db: std::sync::Arc<std::sync::Mutex<Database>>,
    profile: &TaxpayerProfile,
) -> Result<Vec<SubmissionReceipt>, anyhow::Error> {
    let tls = native_tls::TlsConnector::builder().build()?;
    let client = imap::connect((host, 993_u16), host, &tls)?;
    let (mut session, new_access_token) = auth.authenticate(client)?;

    // Save the new access token if it was refreshed
    if let Some(token) = new_access_token {
        let mut updated_profile = profile.clone();
        updated_profile.oauth_access_token = Some(token);
        if let Ok(db_guard) = db.lock() {
            let _ = db_guard.save_profile(updated_profile);
        }
    }

    session.select("INBOX")?;

    // Search ALL BIR confirmation emails from the last 30 days.
    //
    // IMPORTANT: We intentionally do NOT use `UNSEEN` here. If the user reads
    // a confirmation email on their phone or in Gmail web before our background
    // poller runs, the `UNSEEN` filter would silently skip it, causing the
    // draft to stay stuck in "Submitted" forever. Instead, we search ALL
    // matching emails and rely on the `submission_receipts` table's
    // `UNIQUE(filename)` constraint for deduplication — processing the same
    // email twice is harmless (ON CONFLICT DO UPDATE is a no-op for identical data).
    let since_date = {
        let now = chrono::Utc::now().naive_utc().date();
        let since = now - chrono::Duration::days(30);
        since.format("%d-%b-%Y").to_string()
    };
    let search_query = format!("FROM \"ebirforms-noreply@bir.gov.ph\" SINCE {}", since_date);
    let seqs = session.search(&search_query)?;

    let mut processed = Vec::new();

    for seq in seqs {
        let messages = session.fetch(seq.to_string(), "RFC822")?;
        for msg in messages.iter() {
            if let Some(body) = msg.body()
                && let Some(parsed_mail) = mail_parser::MessageParser::default().parse(body)
            {
                let mut text_content = parsed_mail
                    .body_text(0)
                    .map(|s| s.into_owned())
                    .unwrap_or_default();
                let html_content = parsed_mail.body_html(0).map(|s| s.into_owned());

                // Sanitize HTML
                let safe_html =
                    html_content.map(|html| ammonia::Builder::default().clean(&html).to_string());

                // If text_content is empty but we have HTML, use html2text to render it
                if text_content.is_empty()
                    && let Some(html) = &safe_html
                {
                    text_content = html2text::from_read(html.as_bytes(), 80).unwrap_or_default();
                }

                match parse_bir_receipt_email(&text_content, safe_html) {
                    Ok(receipt) => {
                        if let Ok(db_guard) = db.lock()
                            && let Ok((submission_receipt, is_new)) =
                                db_guard.save_submission_receipt(&receipt)
                        {
                            // Confirm the draft if we recognise the form type and it's a new receipt
                            if is_new
                                && (submission_receipt.form_type == "2551Qv2018"
                                    || submission_receipt.form_type == "2551Q")
                            {
                                let _ = db_guard.confirm_2551q_from_receipt(&submission_receipt);

                                // Send OS notification
                                if let Some((_, _, period)) =
                                    crate::receipt::split_bir_filename(&submission_receipt.filename)
                                    && let Some((year, quarter)) =
                                        crate::db::parse_2551q_period(&period)
                                {
                                    crate::notification::send_notification(
                                        "BIR Confirmation Received",
                                        &format!(
                                            "Form: 2551Q\nYear: {}\nQuarter: {}",
                                            year, quarter
                                        ),
                                    );
                                }
                            }
                            processed.push(submission_receipt);
                        }
                    }
                    Err(e) => {
                        // If it's truly an email from BIR but we failed to parse it, log it.
                        if text_content.contains("This confirms receipt of your submission") {
                            tracing::error!(
                                "Failed to parse BIR receipt email. Error: {:?}\nRaw Body snippet: {:.200}",
                                e,
                                text_content
                            );
                        }
                    }
                }
            }
        }
    }

    session.logout()?;
    Ok(processed)
}

/// Fetch emails for a specific email address, across all profiles.
/// Returns (poll_success, still_pending_forms, error_message).
pub fn fetch_and_process_emails_for_address(
    email_address: &str,
    db: std::sync::Arc<std::sync::Mutex<Database>>,
) -> (bool, bool, Option<String>) {
    let (profile, still_pending) = {
        let db_guard = match db.lock() {
            Ok(g) => g,
            Err(e) => return (false, false, Some(format!("DB lock failed: {}", e))),
        };
        let current_year = chrono::Utc::now().naive_utc().date().year() as u16;
        let profiles = db_guard.list_profiles().unwrap_or_default();

        let mut still_pending = false;
        let mut matched_profile = None;

        for p in profiles {
            let p_email = p.imap_email.clone().unwrap_or_else(|| p.email.clone());
            if p_email == email_address {
                if matched_profile.is_none() {
                    matched_profile = Some(p.clone());
                }
                if let Ok(summaries) = db_guard.list_draft_summaries(&p.tin.full(), current_year)
                    && summaries
                        .iter()
                        .any(|s| s.status == crate::forms::FilingStatus::Submitted)
                {
                    still_pending = true;
                }
            }
        }

        (matched_profile, still_pending)
    };

    if !still_pending {
        return (true, false, None);
    }

    if let Some(profile) = profile {
        match fetch_and_process_emails(&profile, db.clone()) {
            Ok(_) => {
                let db_guard = match db.lock() {
                    Ok(g) => g,
                    Err(_) => return (false, true, Some("DB lock failed after fetch".to_string())),
                };
                let mut remaining_pending = false;
                let current_year = chrono::Utc::now().naive_utc().date().year() as u16;
                let profiles = db_guard.list_profiles().unwrap_or_default();
                for p in profiles {
                    let p_email = p.imap_email.clone().unwrap_or_else(|| p.email.clone());
                    if p_email == email_address
                        && let Ok(summaries) =
                            db_guard.list_draft_summaries(&p.tin.full(), current_year)
                        && summaries
                            .iter()
                            .any(|s| s.status == crate::forms::FilingStatus::Submitted)
                    {
                        remaining_pending = true;
                    }
                }
                (true, remaining_pending, None)
            }
            Err(e) => {
                let err_msg = format!("{}", e);
                tracing::warn!("Email polling failed for {}: {}", email_address, err_msg);
                (false, still_pending, Some(err_msg))
            }
        }
    } else {
        (
            false,
            false,
            Some(format!(
                "No profile found matching email: {}",
                email_address
            )),
        )
    }
}
